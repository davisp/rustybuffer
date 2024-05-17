package rustybuffer

func ExampleAllocBuffers() {
	Configure(8*1024*1024*1024, 2*1024*1024*1024)
	sizes := [...]uint64{5, 10, 15}
	entry := AllocBuffers(sizes[:])
	entry.Release()
}
