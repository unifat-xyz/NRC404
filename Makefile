RFLAGS="-C link-arg=-s"

build: build-nrc404

build-nrc404: nrc404
	rustup target add wasm32-unknown-unknown
	RUSTFLAGS=$(RFLAGS) cargo build -p nrc404 --target wasm32-unknown-unknown --release
	mkdir -p res
	rm ./nrc404/res/nrc404.wasm
	cp target/wasm32-unknown-unknown/release/nrc404.wasm ./nrc404/res/nrc404.wasm
	cp target/wasm32-unknown-unknown/release/nrc404.wasm ./res/nrc404.wasm

release:
	$(call docker_build,_rust_setup.sh)
	mkdir -p res
	cp target/wasm32-unknown-unknown/release/nrc404.wasm res/nrc404_release.wasm

unittest: build
ifdef TC
	RUSTFLAGS=$(RFLAGS) cargo test $(TC) -p nrc404 --lib -- --nocapture
else
	RUSTFLAGS=$(RFLAGS) cargo test -p nrc404 --lib -- --nocapture
endif

test: build
ifdef TF
	RUSTFLAGS=$(RFLAGS) cargo test -p nrc404 --test $(TF) -- --nocapture
else
	RUSTFLAGS=$(RFLAGS) cargo test -p nrc404 --tests -- --nocapture
endif

clean:
	cargo clean
	rm -rf res/

define docker_build
	docker build -t my-nrc404-builder .
	docker run \
		--mount type=bind,source=${PWD},target=/host \
		--cap-add=SYS_PTRACE --security-opt seccomp=unconfined \
		-w /host \
		-e RUSTFLAGS=$(RFLAGS) \
		-i -t my-nrc404-builder \
		/bin/bash $(1)
endef
