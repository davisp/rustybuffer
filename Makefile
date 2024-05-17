
all: build

build:
	cd lib/rustybuffer && cargo build --release
	cp lib/rustybuffer/target/release/librustybuffer.a lib/
	go build

check: build
	go test
