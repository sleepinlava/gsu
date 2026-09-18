import torch
metric = torch.ones((), device='cuda')
values = [metric.item() for step in range(3)]
