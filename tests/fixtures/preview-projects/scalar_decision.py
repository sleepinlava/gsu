import torch

score = torch.ones(1, device="cuda")
for step in range(10):
    if bool(score):
        accept(step)
