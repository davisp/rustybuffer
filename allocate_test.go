package rustybuffer

import "testing"

func ExampleAllocBuffers() {
	Configure(8*1024*1024*1024, 2*1024*1024*1024)
	sizes := [...]uint64{5, 10, 15}
	entry := AllocBuffers(sizes[:])
	entry.Release()
}

func BenchmarkAlloc256MBBuffers(b *testing.B) {
	Configure(8*1024*1024*1024, 2*1024*1024*1024)
	for n := 0; n < b.N; n++ {
		sizes := [...]uint64{256*1024*1024}
		entry := AllocBuffers(sizes[:])
		entry.Release()
	}
}
