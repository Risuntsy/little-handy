# 构建阶段
FROM rust:slim as builder

WORKDIR /app

# 安装必要的构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 复制项目文件
COPY . .

# 构建应用
RUN cargo build --release

# 运行阶段
FROM alpine:3.21

# 安装必要的运行时依赖
RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    libgcc \
    libc6-compat \
    wget

WORKDIR /app

# 从构建阶段复制二进制文件并设置执行权限
COPY --from=builder /app/target/release/temp-file-host /app/
COPY --from=builder /app/config/sample.toml /app/config/sample.toml
RUN chmod +x /app/temp-file-host


# 设置环境变量
ENV RUST_LOG=info
ENV UPLOAD_DIR=/app/uploads

# 暴露端口
EXPOSE 11457

# 运行应用
CMD ["/app/temp-file-host"]