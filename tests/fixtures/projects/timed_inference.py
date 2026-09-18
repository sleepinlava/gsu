import torch
import time
torch.cuda.synchronize()
start = time.monotonic()
output = model(data)
torch.cuda.synchronize()
elapsed = time.monotonic() - start
