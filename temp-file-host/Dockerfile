# 使用官方 Rust 镜像作为构建环境
FROM rust:1.82-slim as builder

# 安装必要的系统依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 设置工作目录
WORKDIR /app

# 复制 Cargo 文件
COPY Cargo.toml Cargo.lock ./
COPY utils-share/ ./utils-share/
COPY temp-file-host/Cargo.toml ./temp-file-host/

# 创建一个虚拟的 main.rs 来缓存依赖
RUN mkdir -p temp-file-host/src && echo "fn main() {}" > temp-file-host/src/main.rs
RUN cargo build --release --bin temp-file-host
RUN rm temp-file-host/src/main.rs

# 复制源代码
COPY temp-file-host/src/ ./temp-file-host/src/

# 构建应用
RUN touch temp-file-host/src/main.rs && cargo build --release --bin temp-file-host

# 运行时镜像
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非特权用户
RUN useradd -r -s /bin/false appuser

# 创建应用目录和上传目录
WORKDIR /app
RUN mkdir -p /app/uploads /app/config

# 复制静态文件和配置
COPY temp-file-host/static/ ./static/
COPY temp-file-host/config/ ./config/

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/temp-file-host ./

# 修改所有权
RUN chown -R appuser:appuser /app

# 切换到非特权用户
USER appuser

# 暴露端口
EXPOSE 8080

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# 启动命令
CMD ["./temp-file-host"]