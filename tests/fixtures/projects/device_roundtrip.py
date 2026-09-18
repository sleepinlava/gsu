import torch
features = torch.ones(8, device='cpu')
returned = features.cuda().cpu()
