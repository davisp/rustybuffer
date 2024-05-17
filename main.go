package main

/*
#cgo LDFLAGS: ./lib/librustybuffer.a
#include "./lib/rustybuffer.h"
*/
import "C"

func main() {
  C.hello(C.CString("world"))
}
