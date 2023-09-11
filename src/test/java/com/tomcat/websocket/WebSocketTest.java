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
        chatId = myWebSocketClient.chatInitResp.getData();
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
        chatReq.setChatId(chatId);
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
        chatReq.setChatId(chatId);
        myWebSocketClient.send(JSONUtil.toJsonStr(chatReq));
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.startTopicResp != null && myWebSocketClient.startTopicResp.getData() != null;
        });

        Assert.assertNotNull(myWebSocketClient.startTopicResp.getData() != null);
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
        await().atMost(30, TimeUnit.SECONDS).until(()-> {
            return myWebSocketClient.chatTopicResp != null && myWebSocketClient.chatTopicResp.getData() != null;
        });

        Assert.assertNotNull(myWebSocketClient.chatTopicResp.getData() != null);
        myWebSocketClient.close();
    }


}
