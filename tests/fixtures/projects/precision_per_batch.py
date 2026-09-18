import torch
features = torch.ones(8, dtype=torch.float32)
for epoch in range(3):
    batch = features.half()
