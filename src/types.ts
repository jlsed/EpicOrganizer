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
