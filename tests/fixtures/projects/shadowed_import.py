import torch
def plugin(torch):
    torch.cuda.synchronize()
