class Sample:
    def cuda(self, index=0):
        return self
    def item(self):
        return 1
sample = Sample()
for step in range(3):
    sample.cuda(0).item()
