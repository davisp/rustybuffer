package main

import "fmt"
import "runtime"
import "unsafe"

/*
#cgo LDFLAGS: ./lib/librustybuffer.a
#include <stdint.h>
#include "./lib/rustybuffer.h"
*/
import "C"

// max_total_size - The total number of bytes hat RustyBuffers will allocate
// max_buffer_size - The maximum number of bytes in a single buffer
func Configure(max_total_size uint64, max_buffer_size uint64) {
  c_max_total := C.uint64_t(max_total_size)
  c_max_buffer := C.uint64_t(max_buffer_size)
  res := C.rustybuffer_config(c_max_total, c_max_buffer)
  if res != 0 {
    panic("something something return (nil, err) thing")
  }
}

type RBEntry struct {
  Data unsafe.Pointer
  Buffers [][]uint8
}

func NewRBEntry(data unsafe.Pointer, buffers [][]uint8) RBEntry {
  ret := RBEntry{data, buffers}
  runtime.SetFinalizer(ret, ret.Release)

  return ret
}

func (entry *RBEntry) Release() {
  if entry.Data == nil {
    return
  }

  res := C.rustybuffer_release(entry.Data)

  if res != 0 {
    panic("a thing broke")
  }

  entry.Data = nil
  entry.Buffers = make([][]uint8, 0)
}

func AllocBuffers(sizes []uint64) RBEntry {
  fmt.Println("[Go]:", sizes)

  var num_bytes uint64 = 0
  for _, size := range sizes {
    num_bytes += size
  }

  fmt.Println("[Go]: Total Bytes:", num_bytes)

  c_num_bytes := C.uint64_t(num_bytes)
  var data unsafe.Pointer

	res := C.rustybuffer_acquire(c_num_bytes, &data)

	if res != 0 {
    panic("lol error handling")
	}

  var curr_offset uint64 = 0
  var buffers [][]uint8 = make([][]uint8, len(sizes))
  for idx, size := range sizes {
    ptr := unsafe.Add(data, curr_offset)
    buffers[idx] = unsafe.Slice((*uint8)(ptr), size)
    curr_offset += size
  }

  b := RBEntry{data, buffers}

  return b
}

func main() {
  Configure(8 * 1024 * 1024 * 1024, 2 * 1024 * 1024 * 1024)
  sizes := [...]uint64{5, 10, 15}
	entry := AllocBuffers(sizes[:])
	fmt.Println("[Go]: Entry:", entry)
  entry.Release()
}
