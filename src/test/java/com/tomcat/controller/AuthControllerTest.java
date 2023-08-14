package com.tomcat.controller;

import com.tomcat.controller.requeset.AuthenticationRequest;
import com.tomcat.utils.JsonUtil;
import com.tomcat.utils.JwtUtil;
import org.junit.jupiter.api.Test;
import org.junit.runner.RunWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.context.junit4.SpringRunner;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.MvcResult;
import org.springframework.test.web.servlet.ResultHandler;
import org.springframework.test.web.servlet.request.MockMvcRequestBuilders;
import org.springframework.test.web.servlet.result.MockMvcResultMatchers;

@SpringBootTest
@RunWith(SpringRunner.class)
@AutoConfigureMockMvc
class AuthControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @Autowired
    private JwtUtil jwtUtil;

    @Autowired
    private JsonUtil jsonUtil;

    @Value("${jwt.tokenHeader}")
    private String tokenHeader;
    @Value("${jwt.tokenHead}")
    private String tokenHead;


    @Test
    void findByDeviceIdSuccess() throws Exception{
        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        String deviceId = "8888888";
        String token = jwtUtil.generateToken("", request.username);
        String json = jsonUtil.toJson(request);
        mockMvc.perform(MockMvcRequestBuilders
                        .get("/auth/findByDeviceId?deviceId="+deviceId)
                        .content(json.getBytes())
                        .contentType(MediaType.APPLICATION_JSON_VALUE)
                        .header(tokenHeader,tokenHead+token)
                ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }


    @Test
    void findByDeviceIdFail() throws Exception{
        // 模拟没有token的请求
        AuthenticationRequest request = new AuthenticationRequest("tom", "1234", "", "13823232232", "8888888");
        String json = jsonUtil.toJson(request);
        mockMvc.perform(MockMvcRequestBuilders
                .post("/auth/findByDeviceId")
                .content(json.getBytes())
                .accept(MediaType.APPLICATION_JSON)
                .contentType(MediaType.APPLICATION_JSON_VALUE)
        ).andExpect(MockMvcResultMatchers.status().is(403))
                .andDo(new ResultHandler() {
            @Override
            public void handle(MvcResult result) throws Exception {
                System.out.println("Response body: " + result.getResponse().getContentAsString());
            }
        });
    }


    @Test
    void registerOrLogin() throws Exception{
        AuthenticationRequest request = new AuthenticationRequest("tom1", "1234", "431@qq.com", "13823232231", "8888887");
        String json = jsonUtil.toJson(request);
        mockMvc.perform(MockMvcRequestBuilders
                .post("/auth/registerOrLogin")
                .content(json.getBytes())
                .accept(MediaType.APPLICATION_JSON)
                .contentType(MediaType.APPLICATION_JSON_VALUE)
        ).andExpect(MockMvcResultMatchers.status().isOk())
                .andDo(new ResultHandler() {
                    @Override
                    public void handle(MvcResult result) throws Exception {
                        System.out.println("Response body: " + result.getResponse().getContentAsString());
                    }
                });
    }
}