import torch
features = torch.ones(8, device='cpu')
batch = features.cuda()
consume(batch)
returned = batch.cpu()
