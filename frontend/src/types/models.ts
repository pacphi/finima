// ── Enums / Union Types ──────────────────────────────────────────────

export type AccountType =
  | 'checking'
  | 'savings'
  | 'credit_card'
  | 'loan'
  | 'investment'
  | 'retirement'
  | 'cash'
  | 'other';

export type FileFormat = 'csv' | 'tsv' | 'ofx' | 'qfx' | 'qbo' | 'qif' | 'xls' | 'xlsx';

export type UploadStatus =
  | 'pending'
  | 'previewing'
  | 'mapping'
  | 'importing'
  | 'categorizing'
  | 'complete'
  | 'error';

export type Frequency =
  | 'daily'
  | 'weekly'
  | 'biweekly'
  | 'monthly'
  | 'quarterly'
  | 'semiannual'
  | 'annual';

// ── Domain Models ────────────────────────────────────────────────────

export interface User {
  id: string;
  email: string;
  display_name: string;
  default_currency: string;
  date_format: string;
  onboarded: boolean;
  created_at: string;
  updated_at: string;
}

export interface Portfolio {
  id: string;
  user_id: string;
  name: string;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface Account {
  id: string;
  portfolio_id: string;
  name: string;
  account_type: AccountType;
  institution: string | null;
  currency: string;
  current_balance: number;
  opening_balance: number;
  is_primary_income: boolean;
  is_archived: boolean;
  notes: string | null;
  last_import_at: string | null;
  transaction_count: number;
  created_at: string;
  updated_at: string;
}

export interface Transaction {
  id: string;
  account_id: string;
  upload_id: string | null;
  date: string;
  amount: number;
  description: string;
  memo: string | null;
  category: string | null;
  subcategory: string | null;
  merchant_name: string | null;
  llm_confidence: number | null;
  user_overridden: boolean;
  tags: string[];
  notes: string | null;
  dedup_hash: string;
  created_at: string;
  updated_at: string;
}

export interface Upload {
  id: string;
  account_id: string;
  file_name: string;
  file_size: number;
  file_format: FileFormat;
  status: UploadStatus;
  row_count: number | null;
  imported_count: number | null;
  skipped_count: number | null;
  error_message: string | null;
  column_mapping: Record<string, string> | null;
  created_at: string;
  updated_at: string;
}

export interface RecurringGroup {
  id: string;
  user_id: string;
  merchant_name: string;
  normalized_name: string;
  category: string | null;
  average_amount: number;
  frequency: Frequency;
  last_date: string;
  next_expected_date: string | null;
  is_confirmed: boolean;
  is_income: boolean;
  confidence: number;
  created_at: string;
  updated_at: string;
}

export interface Budget {
  id: string;
  user_id: string;
  portfolio_id: string;
  category: string;
  amount: number;
  month: string;
  spent: number;
  created_at: string;
  updated_at: string;
}

export interface SavingsGoal {
  id: string;
  user_id: string;
  name: string;
  target_amount: number;
  current_amount: number;
  target_date: string | null;
  monthly_contribution: number;
  created_at: string;
  updated_at: string;
}

export interface AccountFlow {
  id: string;
  source_account_id: string;
  destination_account_id: string;
  amount: number;
  date: string;
  source_transaction_id: string | null;
  destination_transaction_id: string | null;
  created_at: string;
}

export interface FlowGroup {
  id: string;
  user_id: string;
  source_account_id: string;
  destination_account_id: string;
  average_amount: number;
  frequency: Frequency;
  flow_count: number;
  last_flow_date: string;
  created_at: string;
  updated_at: string;
}

// ── Helper Types ─────────────────────────────────────────────────────

export interface TransactionFilters {
  date_from?: string;
  date_to?: string;
  account_id?: string;
  category?: string;
  search?: string;
  amount_min?: number;
  amount_max?: number;
}

export interface PaginationParams {
  page: number;
  per_page: number;
}

export interface SortParams {
  sort_by: string;
  sort_dir: 'asc' | 'desc';
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
}

export interface UploadPreview {
  upload_id: string;
  file_name: string;
  file_format: FileFormat;
  headers: string[];
  rows: string[][];
  inferred_mapping: Record<string, string>;
  date_format: string | null;
  row_count: number;
}

export interface ColumnMapping {
  mapping: Record<string, string>;
  skip_duplicates: boolean;
  date_format?: string;
}

export const ACCOUNT_TYPE_LABELS: Record<AccountType, string> = {
  checking: 'Checking',
  savings: 'Savings',
  credit_card: 'Credit Card',
  loan: 'Loan',
  investment: 'Investment',
  retirement: 'Retirement',
  cash: 'Cash',
  other: 'Other',
};

export const ACCOUNT_TYPE_ICONS: Record<AccountType, string> = {
  checking: '🏦',
  savings: '💰',
  credit_card: '💳',
  loan: '🏠',
  investment: '📈',
  retirement: '📈',
  cash: '💵',
  other: '📋',
};

export const COLUMN_MAPPING_OPTIONS = [
  '-- Skip --',
  'Date',
  'Amount',
  'Debit',
  'Credit',
  'Description',
  'Memo',
  'Category',
] as const;

export type ColumnMappingTarget = (typeof COLUMN_MAPPING_OPTIONS)[number];

// ── Dashboard Types ─────────────────────────────────────────────────

export interface DashboardSummary {
  net_worth: number;
  monthly_income: number;
  monthly_expenses: number;
  savings_rate: number;
  health_score: number;
  upcoming_bills_count: number;
}

export interface NetWorthPoint {
  date: string;
  total: number;
  assets: number;
  liabilities: number;
}

export interface MonthlyCashFlow {
  month: string;
  income: number;
  expenses: number;
  net: number;
}

export interface CategorySpend {
  category: string;
  amount: number;
  percentage: number;
}

export interface BudgetVsActual {
  category: string;
  limit: number;
  spent: number;
  remaining: number;
  percentage: number;
}

export interface BudgetSuggestion {
  category: string;
  suggested_limit: number;
  avg_monthly: number;
}

export interface HealthScore {
  score: number;
  savings_rate: number;
  debt_ratio: number;
  emergency_months: number;
  spending_trend: number;
}

// ── Money Flow Types ────────────────────────────────────────────────

export interface SankeyData {
  nodes: SankeyNode[];
  links: SankeyLink[];
}

export interface SankeyNode {
  id: string;
  name: string;
  type: string;
}

export interface SankeyLink {
  source: string;
  target: string;
  value: number;
}

export interface OutflowRank {
  account_id: string;
  account_name: string;
  account_type: string;
  monthly_amount: number;
  pct_income: number;
  trend: string;
}

export interface WaterfallData {
  start_balance: number;
  income: number;
  outflows: WaterfallSegment[];
  end_balance: number;
}

export interface WaterfallSegment {
  name: string;
  amount: number;
}
