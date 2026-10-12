(() => {
  // src/audioWorklet.ts
  var maxQueued = 48e3 * 0.2;
  registerProcessor("native-audio", class extends AudioWorkletProcessor {
    queue = [];
    offset = 0;
    queued = 0;
    constructor() {
      super();
      this.port.onmessage = ({ data }) => {
        this.queue.push(data);
        this.queued += data.length / 2;
        while (this.queued > maxQueued && this.queue.length > 1) {
          this.queued -= (this.queue.shift().length - this.offset) / 2;
          this.offset = 0;
        }
      };
    }
    process(_, [[left, right]]) {
      for (let i = 0; i < left.length && this.queue.length; i++) {
        const chunk = this.queue[0];
        left[i] = chunk[this.offset];
        right[i] = chunk[this.offset + 1];
        this.offset += 2;
        this.queued--;
        if (this.offset >= chunk.length) {
          this.queue.shift();
          this.offset = 0;
        }
      }
      return true;
    }
  });
})();
