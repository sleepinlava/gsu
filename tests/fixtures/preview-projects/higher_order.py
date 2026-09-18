import torch


def derivative(loss: torch.Tensor, x: torch.Tensor):
    for step in range(10):
        grad = torch.autograd.grad(loss, x, retain_graph=True, create_graph=True)
        consume(grad)
