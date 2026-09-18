import torch

scores = torch.ones(2, device="cpu")
for step in range(10):
    values = scores.mean().tolist()
    print(values)
