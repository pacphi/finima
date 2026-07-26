import { Routes, Route, Navigate } from 'react-router';
import { useAuthStore } from '@/stores/authStore';
import { AppLayout } from '@/components/layout/AppLayout';
import { SignInPage } from './SignInPage';
import { MagicLinkSentPage } from './MagicLinkSentPage';
import { VerifyPage } from './VerifyPage';
import { OnboardingPage } from './OnboardingPage';
import { DashboardPage } from './DashboardPage';
import { AccountsPage } from './AccountsPage';
import { AccountDetailPage } from './AccountDetailPage';
import { TransactionsPage } from './TransactionsPage';
import { PayeeRulesPage } from './PayeeRulesPage';
import { RecurringPage } from './RecurringPage';
import { FlowsPage } from './FlowsPage';
import { BudgetPage } from './BudgetPage';
import { GoalsPage } from './GoalsPage';
import { NewsPage } from './NewsPage';
import { PortfoliosPage } from './PortfoliosPage';
import { SettingsPage } from './SettingsPage';
import type { ReactNode } from 'react';

function ProtectedRoute({ children }: { children: ReactNode }) {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  if (!isAuthenticated) {
    return <Navigate to="/auth/signin" replace />;
  }
  return <>{children}</>;
}

function RootRedirect() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  return <Navigate to={isAuthenticated ? '/dashboard' : '/auth/signin'} replace />;
}

export function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<RootRedirect />} />

      {/* Auth routes (public) */}
      <Route path="/auth/signin" element={<SignInPage />} />
      <Route path="/auth/verify" element={<VerifyPage />} />
      <Route path="/auth/magic-link-sent" element={<MagicLinkSentPage />} />

      {/* Onboarding (requires auth but no app shell) */}
      <Route
        path="/onboarding"
        element={
          <ProtectedRoute>
            <OnboardingPage />
          </ProtectedRoute>
        }
      />

      {/* Protected routes with app shell */}
      <Route
        element={
          <ProtectedRoute>
            <AppLayout />
          </ProtectedRoute>
        }
      >
        <Route path="/portfolios" element={<PortfoliosPage />} />
        <Route path="/dashboard" element={<DashboardPage />} />
        <Route path="/accounts" element={<AccountsPage />} />
        <Route path="/accounts/:id" element={<AccountDetailPage />} />
        <Route path="/transactions" element={<TransactionsPage />} />
        <Route path="/payee-rules" element={<PayeeRulesPage />} />
        <Route path="/recurring" element={<RecurringPage />} />
        <Route path="/flows" element={<FlowsPage />} />
        <Route path="/budget" element={<BudgetPage />} />
        <Route path="/goals" element={<GoalsPage />} />
        <Route path="/news" element={<NewsPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Route>

      {/* Catch-all */}
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
