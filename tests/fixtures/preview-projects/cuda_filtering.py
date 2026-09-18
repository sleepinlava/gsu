import torch

mask = torch.ones(2, device="cuda")
for step in range(10):
    indices = mask.nonzero()
    consume(indices)
