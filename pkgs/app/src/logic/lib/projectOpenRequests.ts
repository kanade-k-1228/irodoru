/** Keep OS requests until the editor is ready; process them in arrival order. */
export class ProjectOpenRequests {
  private paths: string[] = [];
  private active = false;
  private opening = false;
  private reading: Promise<void> = Promise.resolve();

  constructor(
    private readonly take: () => Promise<string[]>,
    private readonly canOpen: () => boolean,
    private readonly open: (path: string) => Promise<boolean>,
    private readonly report: (cause: unknown) => void,
    private readonly deferred: () => void,
  ) {}

  start() {
    this.active = true;
    this.refresh();
  }

  stop() {
    this.active = false;
  }

  refresh() {
    this.reading = this.reading
      .then(async () => {
        if (!this.active) return;
        this.paths.push(...(await this.take()));
        void this.resume();
      })
      .catch(this.report);
  }

  async resume() {
    if (!this.active || this.opening) return;
    if (this.paths.length && !this.canOpen()) {
      this.deferred();
      return;
    }
    this.opening = true;
    try {
      while (this.active && this.paths.length && this.canOpen()) {
        try {
          if (!(await this.open(this.paths[0]))) break;
        } catch (cause) {
          // Report once, then let later requests proceed.
          this.report(cause);
        }
        this.paths.shift();
      }
    } finally {
      this.opening = false;
    }
  }
}
