import torch


def summarize(scores: torch.Tensor):
    for step in range(10):
        values = scores.cuda().mean().tolist()
        print(values)
