import torch


def prepare(features: torch.Tensor):
    for step in range(10):
        independent_leaf = torch.tensor(features)
        consume(independent_leaf)
