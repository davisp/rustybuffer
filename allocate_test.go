package rustybuffer

import (
	"fmt"
)

func ExampleAllocBuffers() {
	Configure(8*1024*1024*1024, 2*1024*1024*1024)
	sizes := [...]uint64{5, 10, 15}
	entry := AllocBuffers(sizes[:])
	fmt.Println("[Go]: Entry:", entry)
	// Output: Allocating buffers: [5 10 15]
	// Golang total bytes: 30
	// Entry: {0x600001ab4000 [[0 0 0 0 0] [0 0 0 0 0 0 0 0 0 0] [0 0 0 0 0 0 0 0 0 0 0 0 0 0 0]]}
	entry.Release()
}
