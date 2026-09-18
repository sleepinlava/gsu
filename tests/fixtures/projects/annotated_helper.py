import torch
def stage(x: torch.Tensor):
    for step in range(3):
        prepared = x.cuda()
