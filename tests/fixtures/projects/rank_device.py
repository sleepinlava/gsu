import torch
rank = get_local_rank()
device = torch.device('cuda', rank)
weights = torch.ones(8, device=device)
