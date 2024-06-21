# An environment to demonstrate the use of libalibi 
# i use arch btw
FROM archlinux:latest

# Update the package database and install necessary packages
RUN pacman -Syu --noconfirm \
    && pacman -S --noconfirm \
       base-devel \
       git \
       rustup

RUN rustup default stable && cargo new --lib /tmp/libalibi

WORKDIR /tmp/libalibi
COPY lib.rs src/
COPY Cargo.toml .

RUN cargo build --release && \
    echo /tmp/libalibi/target/release/libalibi.so >> /etc/ld.so.preload && \
    printf '$130000\n$150000\n' > hushmoney.log && \
    ln /usr/sbin/sleep /usr/bin/shadynasty

CMD ["/bin/bash"]
