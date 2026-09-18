import torch
for batch in loader:
    batch = batch.to('cuda')
    output = model(batch)
    loss = criterion(output)
    print(loss.item())
