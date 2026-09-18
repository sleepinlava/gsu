import torch
metric = torch.ones((), device='cpu')
for epoch in range(3):
    print(metric.item())
