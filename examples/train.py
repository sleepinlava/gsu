# Static-check demonstration only. GSU never runs this program.
import torch

features = torch.ones(8, device="cpu", dtype=torch.float64)
for step in range(10):
    batch = features.to("cuda:0")
    reduced = batch.float()
    print(reduced.item())
    torch.cuda.synchronize()

round_trip = features.cuda().cpu()
repeated = features.to("cuda:0").to("cuda:0")
