import { readFile } from 'node:fs/promises';
import { connect } from 'node:net';

export type RuntimeDependency = {
  name: string;
  /** Health endpoint expected to return HTTP 2xx. */
  url?: string;
  /** TCP listener to probe when a service has no unauthenticated health endpoint. */
  tcp?: { host: string; port: number };
  /** PID file supplied by the process launcher; never used to terminate a process. */
  pidFile?: string;
  logFile?: string;
  exitCodeFile?: string;
};

async function readOptional(path?: string): Promise<string | undefined> {
  if (!path) return undefined;
  try { return (await readFile(path, 'utf8')).trim(); } catch { return undefined; }
}

async function probeTcp(host: string, port: number): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const socket = connect({ host, port });
    socket.setTimeout(1000);
    socket.once('connect', () => { socket.destroy(); resolve(); });
    socket.once('error', reject);
    socket.once('timeout', () => { socket.destroy(); reject(new Error('TCP timeout')); });
  });
}

/** A failed probe is an environment failure with enough detail to find the launcher logs. */
export async function checkRuntimeHealth(dependencies: RuntimeDependency[]): Promise<void> {
  for (const dependency of dependencies) {
    const details: string[] = [];
    if (dependency.pidFile) {
      const raw = await readOptional(dependency.pidFile);
      const pid = Number(raw);
      if (!raw || !Number.isSafeInteger(pid) || pid <= 0) {
        details.push(`PID 文件缺失或无效: ${dependency.pidFile}`);
      } else {
        try { process.kill(pid, 0); }
        catch (error) {
          if ((error as NodeJS.ErrnoException).code !== 'EPERM') details.push(`进程 PID ${pid} 已退出`);
        }
      }
    }
    if (dependency.url) {
      try {
        const response = await fetch(dependency.url, { signal: AbortSignal.timeout(1500) });
        if (!response.ok) details.push(`HTTP ${response.status} ${dependency.url}`);
      } catch (error) {
        details.push(`HTTP ${dependency.url}: ${(error as Error).message}`);
      }
    }
    if (dependency.tcp) {
      try { await probeTcp(dependency.tcp.host, dependency.tcp.port); }
      catch (error) {
        details.push(`TCP ${dependency.tcp.host}:${dependency.tcp.port}: ${(error as Error).message}`);
      }
    }
    if (details.length) {
      const exitCode = await readOptional(dependency.exitCodeFile);
      throw new Error(`测试环境失效 (${dependency.name}): ${details.join('; ')}; ` +
        `日志: ${dependency.logFile ?? '未配置'}; 退出码: ${exitCode ?? '未知'}`);
    }
  }
}
