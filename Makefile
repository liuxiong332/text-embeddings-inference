integration-tests:
	cargo test --release

cuda-integration-tests:
	cargo test -F text-embeddings-backend-candle/cuda -F text-embeddings-backend-candle/flash-attn -F text-embeddings-router/candle-cuda --release

integration-tests-review:
	cargo insta test --review --release

cuda-integration-tests-review:
	cargo insta test --review --features "text-embeddings-backend-candle/cuda text-embeddings-backend-candle/flash-attn text-embeddings-router/candle-cuda" --release

build-cuda-turing-consul:
	cargo install --path router -F candle-cuda-turing -F http -F consul --no-default-features 

# Example for Turing (T4, RTX 2000 series, ...)
# runtime_compute_cap=75
# docker build . -f Dockerfile-cuda --build-arg CUDA_COMPUTE_CAP=$runtime_compute_cap
gen-dockerfile-cuda-turing:
	sed -i 's/ARG CUDA_COMPUTE_CAP=80/ARG CUDA_COMPUTE_CAP=75/g' Dockerfile-cuda
	cp Dockerfile-cuda Dockerfile

# Example for A10
# runtime_compute_cap=86
gen-dockerfile-cuda:
	sed -i 's/ARG CUDA_COMPUTE_CAP=80/ARG CUDA_COMPUTE_CAP=86/g' Dockerfile-cuda
	cp Dockerfile-cuda Dockerfile