# Standard Library — AI & ML

Tensor operations, model inference, and embedding utilities. GPU dispatch integrates with the concurrency model.

---

## Tensors

```
let tensor = tensor.from([[1, 2], [3, 4]])
let result = tensor.multiply(other)
let normalized = tensor.normalize()
```

---

## Model Inference

```
let model = model.load("model.onnx")
let prediction = model.predict(inputTensor)
```

---

## Embeddings & Vector Search

```
let embedding = vectors.fromArray([0.1, 0.2, 0.3])
let similarity = embedding.cosineSimilarity(other)
let nearest = embeddings.findNearest(query, count: 5)
```

---

## Expansion Candidates

- GPU tensor operations (integrates with GPU dispatch spec)
- Training loop helpers
- Automatic differentiation (autograd)
- Common layer types (linear, conv, attention)
- Dataset loading and batching
- Model serialization (save/load)
- Tokenizers for NLP
- Image preprocessing
- Audio preprocessing
- Pre-trained model hub integration
- Quantization support
