import torch

predictions = torch.empty(0, device="cuda")
for step in range(10):
    prediction = infer(step)
    predictions = torch.cat([predictions, prediction])
