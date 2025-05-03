doc:
    RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --document-private-items --all-features

test:
    cargo test --all-features

check:
    cargo +nightly fmt --check
    cargo +nightly clippy --all-targets --all-features -- -D warnings
    cargo deny check
    taplo fmt --check

fmt:
    cargo +nightly fmt
    taplo fmt