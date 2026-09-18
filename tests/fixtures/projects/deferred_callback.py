import torch
features = torch.ones(8, device='cpu')
for epoch in range(3):
    callback = lambda: features.cuda()
