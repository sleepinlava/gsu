from torch.cuda import empty_cache

for step in range(10):
    train_step()
    empty_cache()
