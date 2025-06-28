# 使用官方 Rust 镜像作为构建环境
FROM rust:1.82-slim as builder

# 安装必要的系统依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 设置工作目录
WORKDIR /app

# 复制 Cargo 文件
COPY Cargo.toml Cargo.lock ./
COPY utils-share/ ./utils-share/
COPY curl2url/Cargo.toml ./curl2url/

# 创建一个虚拟的 main.rs 来缓存依赖
RUN mkdir -p curl2url/src && echo "fn main() {}" > curl2url/src/main.rs
RUN cargo build --release --bin curl2url
RUN rm curl2url/src/main.rs

# 复制源代码
COPY curl2url/src/ ./curl2url/src/

# 构建应用
RUN touch curl2url/src/main.rs && cargo build --release --bin curl2url

# 运行时镜像
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非特权用户
RUN useradd -r -s /bin/false appuser

# 创建应用目录
WORKDIR /app

# 复制配置文件目录
COPY curl2url/config/ ./config/

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/curl2url ./

# 修改所有权
RUN chown -R appuser:appuser /app

# 切换到非特权用户
USER appuser

# 暴露端口
EXPOSE 3000

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/ || exit 1

# 启动命令
CMD ["./curl2url"] 