export interface FolderEntry {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
}

export interface ProposedOperation {
  tool: string;
  arguments: Record<string, unknown>;
  description: string;
  valid: boolean;
  reason?: string;
}

export interface OrganizePlan {
  root: string;
  instruction: string;
  model: string;
  summary: string;
  operations: ProposedOperation[];
  warnings: string[];
}

export interface OperationRequest {
  tool: string;
  arguments: Record<string, unknown>;
}

export interface OperationResult {
  description: string;
  ok: boolean;
  message: string;
}

export interface ExecutionReport {
  results: OperationResult[];
  okCount: number;
  failedCount: number;
}

export interface ProgressEvent {
  message: string;
}

export interface ConfidentialFinding {
  path: string;
  reasons: string[];
}

export interface DetectionReport {
  findings: ConfidentialFinding[];
  scanned: number;
  warnings: string[];
}

export interface EncryptFileResult {
  path: string;
  output?: string;
  ok: boolean;
  message: string;
}

export interface EncryptReport {
  results: EncryptFileResult[];
  okCount: number;
  failedCount: number;
}

export interface DecryptFileResult {
  path: string;
  output: string;
  ok: boolean;
  message: string;
}
