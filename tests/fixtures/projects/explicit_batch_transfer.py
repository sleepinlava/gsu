import torch
features = torch.ones(8, device='cpu')
for epoch in range(3):
    batch = features.cuda()
    consume(batch)
