import torch


def optimize(loss: torch.Tensor):
    for step in range(10):
        loss.backward(retain_graph=True)
