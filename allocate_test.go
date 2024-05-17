package rustybuffer

func ExampleAllocBuffers() {
	Configure(8*1024*1024*1024, 2*1024*1024*1024)
	sizes := [...]uint64{5, 10, 15}
	entry := AllocBuffers(sizes[:])
	entry.Release()
}

func AllocBuffersSpeed() {
	Configure(8*1024*1024*1024, 2*1024*1024*1024)
	sizes := [...]uint64{256*1024*1024}
	for i := 0; i < 10_000; i++ {
		entry := AllocBuffers(sizes[:])
		entry.Release()
	}
}
