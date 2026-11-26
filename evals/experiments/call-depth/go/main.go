package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"github.com/tetratelabs/wazero"
)

type reply struct {
	Kind    string `json:"kind"`
	Code    string `json:"code"`
	Message string `json:"message"`
	Result  struct {
		Status    string `json:"status"`
		States    int    `json:"states"`
		Generated int    `json:"generated"`
		Work      int    `json:"work"`
	} `json:"result"`
}

func dispatch(ctx context.Context, wasm []byte, request []byte) (string, error) {
	runtime := wazero.NewRuntime(ctx)
	defer runtime.Close(ctx)
	module, err := runtime.Instantiate(ctx, wasm)
	if err != nil {
		return "", err
	}
	resize, err := module.ExportedFunction("modelt_input_resize").Call(ctx, uint64(len(request)))
	if err != nil {
		return "", err
	}
	if !module.Memory().Write(uint32(resize[0]), request) {
		return "", fmt.Errorf("input outside memory")
	}
	if _, err := module.ExportedFunction("modelt_dispatch").Call(ctx); err != nil {
		return fmt.Sprintf("trap: %v", err), nil
	}
	pointer, err := module.ExportedFunction("modelt_output_ptr").Call(ctx)
	if err != nil {
		return "", err
	}
	length, err := module.ExportedFunction("modelt_output_len").Call(ctx)
	if err != nil {
		return "", err
	}
	output, ok := module.Memory().Read(uint32(pointer[0]), uint32(length[0]))
	if !ok {
		return "", fmt.Errorf("output outside memory")
	}
	var parsed reply
	if err := json.Unmarshal(output, &parsed); err != nil {
		return "", err
	}
	if parsed.Kind == "error" {
		return fmt.Sprintf("error %s: %s", parsed.Code, parsed.Message), nil
	}
	return fmt.Sprintf("%s %d %d %d", parsed.Result.Status, parsed.Result.States, parsed.Result.Generated, parsed.Result.Work), nil
}

func main() {
	if len(os.Args) < 3 {
		fmt.Fprintln(os.Stderr, "usage: call-depth-probe <wasm> <request.json>...")
		os.Exit(2)
	}
	wasm, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	for _, path := range os.Args[2:] {
		request, err := os.ReadFile(path)
		if err != nil {
			panic(err)
		}
		outcome, err := dispatch(context.Background(), wasm, request)
		if err != nil {
			panic(err)
		}
		fmt.Printf("%s %s\n", filepath.Base(path), outcome)
	}
}
