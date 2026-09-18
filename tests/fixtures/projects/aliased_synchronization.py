from torch.cuda import synchronize as wait_for_gpu
for epoch in range(3):
    wait_for_gpu()
