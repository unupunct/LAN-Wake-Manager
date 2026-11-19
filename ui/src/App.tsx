import { HashRouter, Navigate, Route, Routes } from 'react-router-dom';
import { Layout } from './components/Layout';
import { Boot, Login, Setup, Unreachable } from './pages/Auth';
import { AppProvider, useApp } from './store';
import { Dashboard } from './pages/Dashboard';
import { DevicesPage } from './pages/Devices';
import { DeviceForm } from './pages/DeviceForm';
import { Discovery } from './pages/Discovery';
import { GroupsPage } from './pages/Groups';
import { LocationsPage } from './pages/Locations';
import { SchedulesPage } from './pages/Schedules';
import { HistoryPage } from './pages/History';
import { MobilePage } from './pages/Mobile';
import { RemotePage } from './pages/Remote';
import { BackupPage } from './pages/Backup';
import { AuditPage } from './pages/Audit';
import { SettingsPage } from './pages/Settings';

function Gate() {
  const { auth } = useApp();
  if (auth === 'loading') return <Boot />;
  if (auth === 'unreachable') return <Unreachable />;
  if (auth === 'setup') return <Setup />;
  if (auth === 'login') return <Login />;
  return (
    <HashRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="devices" element={<DevicesPage />} />
          <Route path="devices/new" element={<DeviceForm />} />
          <Route path="devices/:id/edit" element={<DeviceForm />} />
          <Route path="devices/:id" element={<DevicesPage />} />
          <Route path="discovery" element={<Discovery />} />
          <Route path="groups" element={<GroupsPage />} />
          <Route path="locations" element={<LocationsPage />} />
          <Route path="schedules" element={<SchedulesPage />} />
          <Route path="history" element={<HistoryPage />} />
          <Route path="mobile" element={<MobilePage />} />
          <Route path="remote" element={<RemotePage />} />
          <Route path="backup" element={<BackupPage />} />
          <Route path="audit" element={<AuditPage />} />
          <Route path="settings" element={<SettingsPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}

export default function App() {
  return (
    <AppProvider>
      <Gate />
    </AppProvider>
  );
}
