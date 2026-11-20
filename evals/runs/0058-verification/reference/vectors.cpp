#include <cstdio>
#include <cinttypes>
#include "MurmurHash3.cpp"
int main() {
    for (uint64_t input : {UINT64_C(0), UINT64_C(1), UINT64_C(0x8000000000000000), UINT64_C(0xffffffffffffffff), UINT64_C(0x0123456789abcdef), UINT64_C(0x9e3779b97f4a7c15), UINT64_C(0x181c6c6b00000000)}) {
        std::printf("0x%016" PRIx64 " 0x%016" PRIx64 "\n", input, fmix64(input));
    }
}
