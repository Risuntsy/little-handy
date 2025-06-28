use std::process::Command;
use std::{env::current_dir, path::Path};

use git2::{build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks, Repository};
use log::{error, info};

use crate::config::{get_config, get_service_config};
use anyhow::{bail, Result};

pub trait GitClient {
    fn clone_or_pull_repo(
        &self,
        repo_url: &str,
        repo_path: &Path,
        private_key_path: Option<&Path>,
    ) -> Result<()> {
        if let Err(e) = if repo_path.exists() && repo_path.is_dir() {
            self.pull_repo(repo_path, private_key_path)
        } else {
            self.clone_repo(repo_url, repo_path, private_key_path)
        } {
            error!("failed to clone or pull repo, error: {}", e)
        }

        Ok(())
    }

    fn clone_repo(
        &self,
        repo_url: &str,
        repo_path: &Path,
        private_key_path: Option<&Path>,
    ) -> Result<()>;

    fn pull_repo(&self, repo_path: &Path, private_key_path: Option<&Path>) -> Result<()>;
}

struct GitClientGit2Impl;

impl GitClientGit2Impl {
    fn build_fetch_options(private_key_path: &Path) -> FetchOptions {
        let mut remote_callbacks = RemoteCallbacks::new();
        remote_callbacks.credentials(move |_url, username_from_url, _allowed_types| {
            Cred::ssh_key(
                username_from_url.unwrap_or("git"),
                None,
                private_key_path,
                None,
            )
        });

        remote_callbacks.certificate_check(|_cert, _hostname| {
            // WARNING: This disables host key verification.
            Ok(git2::CertificateCheckStatus::CertificateOk)
        });

        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(remote_callbacks);

        fetch_options
    }

    fn build_repo_builder(private_key_path: Option<&Path>) -> RepoBuilder {
        if private_key_path.is_none() {
            return RepoBuilder::new();
        }

        let mut repo_builder = RepoBuilder::new();
        repo_builder.fetch_options(Self::build_fetch_options(private_key_path.unwrap()));
        repo_builder
    }
}

impl GitClient for GitClientGit2Impl {
    fn clone_repo(
        &self,
        repo_url: &str,
        repo_path: &Path,
        private_key_path: Option<&Path>,
    ) -> Result<()> {
        let mut builder = Self::build_repo_builder(private_key_path);
        let _ = builder
            .clone(repo_url, repo_path)
            .expect("Failed to clone repo");
        Ok(())
    }

    fn pull_repo(&self, repo_path: &Path, private_key_path: Option<&Path>) -> Result<()> {
        let repo = Repository::open(repo_path)?;
        let mut remote = repo.find_remote("origin")?;

        let mut fetch_option = if let Some(private_key_path) = private_key_path {
            Self::build_fetch_options(private_key_path)
        } else {
            git2::FetchOptions::new()
        };

        remote.fetch(
            &["refs/heads/*:refs/remotes/origin/*"],
            Some(&mut fetch_option),
            None,
        )?;

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
        let analysis = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.0.is_up_to_date() {
            info!("Repository is up to date.");
            Ok(())
        } else if analysis.0.is_fast_forward() {
            let refname = format!("refs/heads/{}", "main"); // Replace "main" with your branch name if needed
            let mut reference = repo.find_reference(&refname)?;
            reference.set_target(fetch_commit.id(), "Fast-forward")?;
            repo.set_head(&refname)?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            info!("Repository fast-forwarded.");
            Ok(())
        } else {
            Err(git2::Error::from_str("Not a fast-forward merge").into())
        }
    }
}

struct GitClientGitImpl;

impl GitClientGitImpl {
    fn new() -> Self {
        // git config pull.rebase true
        Command::new("git")
            .arg("config")
            .arg("pull.rebase")
            .arg("true")
            .spawn()
            .expect("Failed to set git config");
        Self
    }

    pub fn exists_git() -> bool {
        let output = Command::new("git").arg("--version").output();

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    fn set_git_env(command: &mut Command, private_key_path: Option<&Path>) {
        if let Some(private_key_path) = private_key_path {
            let private_key_path_str = current_dir()
                .expect("Failed to get current directory")
                .join(private_key_path)
                .to_str()
                .expect("Invalid private key path")
                // shit windows
                .replace("\\", "/");
            info!("Using private key: {}", private_key_path_str);
            command.env(
                "GIT_SSH_COMMAND",
                format!(
                    "ssh -i {} -o IdentitiesOnly=yes -o StrictHostKeyChecking=no",
                    private_key_path_str
                ),
            );
        }
    }

    pub fn clone_with_git(
        repo_url: &str,
        repo_path: &Path,
        private_key_path: Option<&Path>,
    ) -> Result<()> {
        let mut command = Command::new("git");
        command.arg("clone");

        Self::set_git_env(&mut command, private_key_path);

        command.arg(repo_url).arg(repo_path);

        let status = command.status()?;

        if status.success() {
            Ok(())
        } else {
            bail!("Failed to clone repository using git command");
        }
    }

    pub fn pull_with_git(repo_path: &Path, private_key_path: Option<&Path>) -> Result<()> {
        let mut command = Command::new("git");
        command.current_dir(repo_path).arg("pull");

        Self::set_git_env(&mut command, private_key_path);

        let status = command.status()?;

        if status.success() {
            Ok(())
        } else {
            bail!("Failed to pull repository using git command");
        }
    }
}

impl GitClient for GitClientGitImpl {
    fn clone_repo(
        &self,
        repo_url: &str,
        repo_path: &Path,
        private_key_path: Option<&Path>,
    ) -> Result<()> {
        Self::clone_with_git(repo_url, repo_path, private_key_path)
    }

    fn pull_repo(&self, repo_path: &Path, private_key_path: Option<&Path>) -> Result<()> {
        Self::pull_with_git(repo_path, private_key_path)
    }
}

pub struct GitClientImpl {
    git_client: Box<dyn GitClient>,
}

// unsafe impl Send for GitClientImpl {}
// unsafe impl Sync for GitClientImpl {}

impl GitClientImpl {
    pub fn new() -> Self {
        let git_client: Box<dyn GitClient> = if GitClientGitImpl::exists_git() {
            Box::new(GitClientGitImpl::new())
        } else {
            Box::new(GitClientGit2Impl)
        };
        GitClientImpl { git_client }
    }
}

impl GitClient for GitClientImpl {
    fn clone_or_pull_repo(
        &self,
        repo_url: &str,
        repo_path: &Path,
        private_key_path: Option<&Path>,
    ) -> Result<()> {
        self.git_client
            .clone_or_pull_repo(repo_url, repo_path, private_key_path)
    }

    fn clone_repo(
        &self,
        repo_url: &str,
        repo_path: &Path,
        private_key_path: Option<&Path>,
    ) -> Result<()> {
        self.git_client
            .clone_repo(repo_url, repo_path, private_key_path)
    }

    fn pull_repo(&self, repo_path: &Path, private_key_path: Option<&Path>) -> Result<()> {
        self.git_client.pull_repo(repo_path, private_key_path)
    }
}

pub fn clone_or_pull_service_repo(service_name: &str) -> Result<()> {

    // TODO folder file lock, concurrency

    let service_config = get_service_config(service_name);

    GitClientImpl::new().clone_or_pull_repo(
        &service_config.repo_url,
        &service_config.repo_path,
        service_config
            .private_key_path
            .as_ref()
            .map(|p| p.as_path()),
    )
}

pub fn init_all_repo() {
    get_config().services.iter().for_each(|service| {
        clone_or_pull_service_repo(&service.name)
            .expect(&format!("init repo failed for {}", &service.name))
    });
}
