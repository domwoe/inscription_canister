import { useEffect, useState } from 'react';
import {stringToBase64} from 'uint8array-extras';
import './App.css';
import { Button, Input, Select, SelectItem } from '@nextui-org/react';
import { backend } from './declarations/backend';

import axios from 'axios';

function App() {
  const [balance, setBalance] = useState<number | undefined>();
  const [walletAddress, setWalletAddress] = useState<string>('');
  const [address, setAddress] = useState<string>('');
  const [content, setContent] = useState<string>('Hello World');
  const [contentType, setContentType] = useState<number>(0);
  const [recipientAddress, setRecipientAddress] = useState<string>('');
  const [currentBlockHeight, setCurrentBlockHeight] = useState<number>(0);
  const [transactions, setTransactions] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [isInscribing, setIsInscribing] = useState(false);

  const btcRpcUrl = 'http://localhost:8000/proxy';
  const rpcUser = 'icp'; // Your RPC username
  const rpcPassword = 'test'; // Your RPC password



  const btcRpcHeaders = {
    'Content-Type': 'application/json',
    Authorization:
      'Basic ' + stringToBase64(rpcUser + ':' + rpcPassword),
  };

  const inscriptionTypes = [
    { value: 'text', label: 'Text', contentType: 'text/plain;charset=utf-8' },
    {
      value: 'json',
      label: 'JSON',
      contentType: 'application/json;charset=utf-8',
    },
  ];

  async function callRPC(method: string, params: any) {
    // RPC command as a JSON object
    const data = {
      jsonrpc: '1.0',
      id: 'curltext',
      method,
      params,
    };

    try {
      const response = await axios.post(btcRpcUrl, data, {
        headers: btcRpcHeaders,
      });

      console.log('RPC Result', response.data.result);
      return response.data.result;
    } catch (error: any) {
      console.error('Error:', error.message);
    }
  }

  const getBlockHeight = async () => {
    try {
      const res = await callRPC('getblockcount', []);
      console.log('Block Height: ', res);
      setCurrentBlockHeight(res);
    } catch (err) {
      console.error(err);
    }
  };

  const getwalletaddress = async () => {
    try {
      const res = await callRPC('getnewaddress', []);
      console.log(res);
      setWalletAddress(res);
    } catch (err) {
      console.error(err);
    }
  };

  const generateBlock = async () => {
    try {
      const res = await callRPC('generatetoaddress', [1, walletAddress]);
      console.log(res);
      await getBlockHeight();
    } catch (err) {
      console.error(err);
    }
  };

  const topup = async () => {
    try {
      const res = await callRPC('sendtoaddress', [address, 1]);
      console.log(res);
      await generateBlock();
      await getBalance();
    } catch (err) {
      console.error(err);
    }
  };

  const getAddress = async () => {
    try {
      setLoading(true);
      const address = await backend.get_p2pkh_address();
      setAddress(address);
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const getBalance = async () => {
    try {
      setLoading(true);
      console.log(address);
      if (address) {
        const bigBalance = await backend.get_balance(address);
        let balance = 0;
        if (bigBalance) {
          balance = Number(bigBalance);
        }

        setBalance(Number(balance)); // Convert BigInt to number
      }
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const handleContentChange = (event: any) => {
    setContent(event.target.value);
  };

  const handleContentTypeChange = (event: any) => {
    const index = inscriptionTypes
      .map((e) => e.value)
      .indexOf(event.target.value);
    setContentType(index);
  };

  const handleRecipientAddressChange = (event: any) => {
    setRecipientAddress(event.target.value);
  };

  const inscribe = async () => {
    console.log(contentType);
    const type = inscriptionTypes[contentType].contentType;
    console.log(content);
    try {
      setIsInscribing(true);
      const result = await backend.inscribe(type, content, [], []);
      console.log(result);
      setTransactions(transactions);
      await generateBlock();
    } catch (err) {
      console.error(err);
    } finally {
      setIsInscribing(false);
    }
  };

  // Fetch balance on load
  useEffect(() => {
    async function init() {
      await getAddress();
      await getwalletaddress();
      await getBlockHeight();
      await getBalance();
    }
    init();
  }, []);

  return (
    <div className="App">
      <h1>Ordinal Inscription Testbed</h1>
      <div className="card">
        <div className="flex w-full flex-col md:flex-nowrap gap-4 space-y-1">
          <Input
            type="text"
            label="Address"
            placeholder={address}
            isReadOnly={true}
          />

          <Input
            type="text"
            label="Balance"
            placeholder={balance + ''}
            isReadOnly={true}
          />
        </div>
        <div className="flex w-full gap-4 my-4">
          <Button
            isLoading={loading}
            isDisabled={!address}
            onClick={getBalance}
          >
            Check Balance
          </Button>
          <Button isDisabled={!address} onClick={topup}>
            Topup
          </Button>
        </div>

        <div className="flex w-full flex-col md:flex-nowrap gap-4">
          <Input
            type="text"
            label="Current Block Height"
            placeholder={currentBlockHeight + ''}
            isReadOnly={true}
          />

          <Button onClick={generateBlock}>Generate Block</Button>

          <Input
            type="text"
            label="Recipient Address"
            onChange={handleRecipientAddressChange}
            value={recipientAddress}
          />

          <Select
            items={inscriptionTypes}
            onChange={handleContentTypeChange}
            label="Content Type"
            defaultSelectedKeys={[inscriptionTypes[contentType].value]}
          >
            {(inscriptionType) => (
              <SelectItem key={inscriptionType.value}>
                {inscriptionType.label}
              </SelectItem>
            )}
          </Select>

          <Input
            type="text"
            label="Inscription Content"
            onChange={handleContentChange}
            value={content}
          />

          <Button color="secondary" isLoading={isInscribing} onClick={inscribe}>
            Inscribe
          </Button>
        </div>
      </div>
    </div>
  );
}

export default App;
