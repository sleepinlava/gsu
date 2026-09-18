import torch
metric = torch.ones((), device='cuda')
for epoch in range(3):
    print(metric.item())
