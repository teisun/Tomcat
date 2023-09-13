package com.tomcat.websocket;

import cn.hutool.json.JSONUtil;
import com.tomcat.controller.requeset.ChatReq;
import lombok.extern.slf4j.Slf4j;
import org.java_websocket.WebSocket;
import org.junit.Assert;
import org.junit.jupiter.api.MethodOrderer;
import org.junit.jupiter.api.Order;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestMethodOrder;
import org.junit.runner.RunWith;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.context.junit4.SpringRunner;

import java.net.URI;
import java.util.concurrent.TimeUnit;

import static org.awaitility.Awaitility.await;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.websocket
 * @className: WebSocketTest
 * @author: tomcat
 * @description: ws 单元测试
 * @date: 2023/9/6 5:49 PM
 * @version: 1.0
 */


@Slf4j
@SpringBootTest
@RunWith(SpringRunner.class)
@AutoConfigureMockMvc
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class WebSocketTest {

    MyWebSocketClient myWebSocketClient;

    private static String uId;
    private static String chatId;

    /**
     * @description 测试websocket连接
     * @param :
     * @return void
     * @author tomcat
     * @date 2023/9/6 6:12 PM
     */
    @Order(1)
    @Test
    public void _1websocketClientConnectTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });

        Assert.assertEquals(WebSocket.READYSTATE.OPEN, myWebSocketClient.getReadyState());
        myWebSocketClient.close();
    }

    @Order(2)
    @Test
    public void _2chatInitTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CHAT_INIT);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatInitResp.getData() != null;
        });

        Assert.assertNotNull(myWebSocketClient.chatInitResp.getData());
        uId = myWebSocketClient.chatInitResp.getData();
        myWebSocketClient.close();
    }

    @Order(3)
    @Test
    public void _3curriculumPlanTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CURRICULUM_PLAN);
        chatReq.setUid(uId);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.planResp != null && myWebSocketClient.planResp.getData() != null;
        });

        Assert.assertNotNull(myWebSocketClient.planResp.getData() != null);
        myWebSocketClient.close();
    }

    @Order(4)
    @Test
    public void _4startTopicTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.START_TOPIC);
        chatReq.setData("At the Restaurant");
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.startTopicResp != null && myWebSocketClient.startTopicResp.getData() != null;
        });
        chatId = myWebSocketClient.startTopicResp.getChatId();
        // msg confirm
        ChatReq confirmReq = new ChatReq();
        confirmReq.setCommand(Command.MSG_CONFIRM);
        confirmReq.setData(myWebSocketClient.startTopicResp.getMsgId());
        myWebSocketClient.send(JSONUtil.toJsonStr(confirmReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.confirmResp != null && myWebSocketClient.confirmResp.getCode() == 200;
        });

        myWebSocketClient.close();
    }

    @Order(5)
    @Test
    public void _5chatTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CHAT);
        chatReq.setData("Hello!Good to see you! I'm Tom!");
        chatReq.setChatId(chatId);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(60, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatTopicResp != null && myWebSocketClient.chatTopicResp.getData() != null;
        });

        // msg confirm
        ChatReq confirmReq = new ChatReq();
        confirmReq.setCommand(Command.MSG_CONFIRM);
        confirmReq.setData(myWebSocketClient.chatTopicResp.getMsgId());
        myWebSocketClient.send(JSONUtil.toJsonStr(confirmReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.confirmResp != null && myWebSocketClient.confirmResp.getCode() == 200;
        });

        myWebSocketClient.close();
    }

    @Order(6)
    @Test
    public void _6offlineMsgTest() throws Exception{
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        ChatReq chatReq = new ChatReq();
        chatReq.setCommand(Command.CHAT);
        chatReq.setData("Hello!Good to see you! I'm Tom!");
        chatReq.setChatId(chatId);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        Thread.sleep(200);
        log.info(" _6offlineMsgTest: 关闭ws链接");
        myWebSocketClient.close();
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.CLOSED);
        });
        Thread.sleep(1000 * 5);
        myWebSocketClient = new MyWebSocketClient(new URI("ws://localhost:6688/ws?Authorization=BearereyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiLmsaTlp4bnjKsiLCJ1c2VySWQiOiIyYzg5ODllYy0yMzRhLTQ5M2EtOWQ4NS1hYTQ0ZmIwNDViNWYiLCJ1c2VybmFtZSI6IuaxpOWnhueMqyIsImlhdCI6MTY5Mjc3MDgyOCwiZXhwIjoxNzI0MzA2ODI4fQ.ATEPOdBcOpN_AU69LQ_2LdVn5XRGzjASBxy4W4POIAc"));
        myWebSocketClient.connect();
        log.info(" _6offlineMsgTest: 重连");
        await().atMost(5, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.getReadyState().equals(WebSocket.READYSTATE.OPEN);
        });
        log.info(" _6offlineMsgTest: 重连成功");
        ChatReq chatReq1 = new ChatReq();
        chatReq1.setCommand(Command.OFFLINE_MSG);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq1));
        await().atMost(10, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatOffMsgResp != null && myWebSocketClient.chatOffMsgResp.getData() != null;
        });
        log.info(" _6offlineMsgTest offline msg: " + JSONUtil.toJsonStr(myWebSocketClient.chatOffMsgResp.getData()));
        Assert.assertNotNull(myWebSocketClient.chatOffMsgResp.getData() != null);
        myWebSocketClient.close();
    }


}
