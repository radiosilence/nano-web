package main

import (
	"bytes"
	"compress/gzip"
	"sync"

	"github.com/andybalholm/brotli"
)

var bufferPool = sync.Pool{
	New: func() interface{} {
		return &bytes.Buffer{}
	},
}

func gzipData(dat []byte) []byte {
	buffer := bufferPool.Get().(*bytes.Buffer)
	defer func() {
		buffer.Reset()
		bufferPool.Put(buffer)
	}()

	writer := gzip.NewWriter(buffer)
	writer.Write(dat)
	writer.Close()

	result := make([]byte, buffer.Len())
	copy(result, buffer.Bytes())
	return result
}

func brotliData(dat []byte) []byte {
	buffer := bufferPool.Get().(*bytes.Buffer)
	defer func() {
		buffer.Reset()
		bufferPool.Put(buffer)
	}()

	writer := brotli.NewWriter(buffer)
	writer.Write(dat)
	writer.Close()

	result := make([]byte, buffer.Len())
	copy(result, buffer.Bytes())
	return result
}

func getAcceptedEncoding(acceptEncoding []byte) string {
	if bytes.Contains(acceptEncoding, brEncoding) {
		return "br"
	}
	if bytes.Contains(acceptEncoding, gzipEncoding) {
		return "gzip"
	}
	return ""
}