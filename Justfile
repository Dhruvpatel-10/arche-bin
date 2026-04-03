# arche-bin — custom system binaries for arche
# Built binaries deploy to ~/arche/tools/bin/

arche := env("HOME") + "/arche"

# Build all binaries (release)
build: build-greeter build-legion

# Build arche-greeter
build-greeter:
    cargo build --release -p arche-greeter

# Build arche-legion
build-legion:
    cargo build --release -p arche-legion

# Run all tests
test:
    cargo test --workspace

# Build and deploy all binaries to arche/tools/bin/
deploy: build
    mkdir -p {{arche}}/tools/bin
    cp target/release/arche-greeter {{arche}}/tools/bin/
    cp target/release/arche-legion {{arche}}/tools/bin/
    @echo "Deployed to {{arche}}/tools/bin/"

# Deploy greeter only
deploy-greeter: build-greeter
    mkdir -p {{arche}}/tools/bin
    cp target/release/arche-greeter {{arche}}/tools/bin/
    @echo "Deployed arche-greeter"

# Deploy legion only
deploy-legion: build-legion
    mkdir -p {{arche}}/tools/bin
    cp target/release/arche-legion {{arche}}/tools/bin/
    @echo "Deployed arche-legion"

# Check workspace compiles without building
check:
    cargo check --workspace

# Format all code
fmt:
    cargo fmt --all

# Lint all code
lint:
    cargo clippy --workspace -- -D warnings
