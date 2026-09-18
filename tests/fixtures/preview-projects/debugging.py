from torch.autograd import detect_anomaly

with detect_anomaly():
    train_step()
