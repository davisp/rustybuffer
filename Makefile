
.PHONY: build-static
build-static:
	cd lib/rustybuffer && cargo build --release
	cp lib/rustybuffer/target/release/librustybuffer.a lib/
	go build main.go
